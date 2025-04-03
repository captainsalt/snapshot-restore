use aws_sdk_ec2::{
    Client,
    client::Waiters,
    types::{Filter, Instance, InstanceBlockDeviceMapping, Snapshot, SnapshotState, Volume},
};
use futures::future::join_all;
use std::time::Duration;
mod aws_err;
use aws_err::ApplicationError;

const WAIT_DURATION: Duration = Duration::from_secs(3600); // hour

async fn get_instances(
    ec2_client: &Client,
    filters: Option<Vec<Filter>>,
) -> Result<Vec<Instance>, ApplicationError> {
    let response = ec2_client
        .describe_instances()
        .set_filters(filters)
        .send()
        .await
        .map_err(|err| ApplicationError::from_err("Failed to describe instances", err))?;

    Ok(response
        .reservations
        .unwrap_or_default()
        .into_iter()
        .flat_map(|reservation| reservation.instances.unwrap_or_default())
        .collect())
}

#[allow(dead_code)]
pub async fn find_instances_by_id(
    ec2_client: &Client,
    instance_ids: Vec<String>,
) -> Result<Vec<Instance>, ApplicationError> {
    get_instances(
        ec2_client,
        Some(vec![
            Filter::builder()
                .name("instance-id")
                .set_values(Some(instance_ids))
                .build(),
        ]),
    )
    .await
}

pub async fn find_instances_by_name(
    ec2_client: &Client,
    instance_names: Vec<String>,
) -> Result<Vec<Instance>, ApplicationError> {
    get_instances(
        ec2_client,
        Some(vec![
            Filter::builder()
                .name("tag:Name")
                .set_values(Some(instance_names.iter().map(|s| s.to_string()).collect()))
                .build(),
        ]),
    )
    .await
}

#[allow(dead_code)]
pub async fn stop_instance(
    ec2_client: &Client,
    instance: &Instance,
) -> Result<(), ApplicationError> {
    let instance_id = instance
        .instance_id
        .as_deref()
        .ok_or_else(|| ApplicationError::new("Missing instance ID"))?;

    ec2_client
        .stop_instances()
        .instance_ids(instance_id)
        .send()
        .await
        .map_err(|err| {
            ApplicationError::from_err(&format!("Error stopping instance {}", instance_id), err)
        })?;

    ec2_client
        .wait_until_instance_stopped()
        .instance_ids(instance_id)
        .wait(WAIT_DURATION)
        .await
        .map_err(|err| {
            ApplicationError::from_err(
                &format!("Error waiting for instance {} to stop", instance_id),
                err,
            )
        })?;

    Ok(())
}

#[allow(dead_code)]
pub async fn start_instance(
    ec2_client: &Client,
    instance: &Instance,
) -> Result<(), ApplicationError> {
    let instance_id = instance
        .instance_id
        .as_deref()
        .ok_or_else(|| ApplicationError::new("Missing instance ID"))?;

    ec2_client
        .start_instances()
        .instance_ids(instance_id)
        .send()
        .await
        .map_err(|err| {
            ApplicationError::from_err(&format!("Error starting instance {}", instance_id), err)
        })?;

    ec2_client
        .wait_until_instance_status_ok()
        .instance_ids(instance_id)
        .wait(WAIT_DURATION)
        .await
        .map_err(|err| {
            ApplicationError::from_err(
                &format!("Error waiting for instance {} to start", instance_id),
                err,
            )
        })?;

    Ok(())
}

pub async fn get_instance_snapshots(
    ec2_client: &Client,
    instance: &Instance,
) -> Result<Vec<Snapshot>, ApplicationError> {
    // Extract volume IDs from instance
    let mut volume_ids = Vec::new();

    for device in instance.block_device_mappings() {
        let ebs = device
            .ebs()
            .ok_or_else(|| ApplicationError::new("Instance should have EBS volume attached"))?;

        let volume_id = ebs
            .volume_id()
            .ok_or_else(|| ApplicationError::new("Volume should have ID"))?
            .to_string();

        volume_ids.push(volume_id);
    }

    // Get snapshots for these volumes
    let snapshots = ec2_client
        .describe_snapshots()
        .filters(
            Filter::builder()
                .name("volume-id")
                .set_values(Some(volume_ids.clone()))
                .build(),
        )
        .send()
        .await
        .map_err(|err| ApplicationError::from_err("Failed to describe snapshots", err))?;

    Ok(snapshots.snapshots.unwrap_or_default())
}

pub async fn get_most_recent_snapshots<'a>(
    instance: &'a Instance,
    snapshots: &Vec<Snapshot>,
) -> Result<Vec<Snapshot>, ApplicationError> {
    let instance_block_devices = instance.block_device_mappings();
    let mut snapshots: Vec<Snapshot> = snapshots.to_owned();

    // Sort snapshots by start time (newest first)
    snapshots.sort_by(|a, b| {
        let a_time = a.start_time().expect("Snapshot should have start time");
        let b_time = b.start_time().expect("Snapshot should have start time");
        b_time.cmp(&a_time)
    });

    // Helper function to get volume ID from device mapping
    let get_volume_id =
        |device_mapping: &'a InstanceBlockDeviceMapping| -> Result<&str, ApplicationError> {
            let ebs = device_mapping
                .ebs()
                .ok_or_else(|| ApplicationError::new("EBS should exist"))?;

            ebs.volume_id()
                .ok_or_else(|| ApplicationError::new("Volume ID should exist if EBS exists"))
        };

    // Build instance volume IDs
    let mut instance_volume_ids = Vec::new();
    for device in instance_block_devices {
        let volume_id = get_volume_id(device)?;
        instance_volume_ids.push(volume_id);
    }

    // Filter for completed snapshots for our volumes
    let instance_snapshots = snapshots
        .into_iter()
        .filter(|snap| snap.state() == Some(&SnapshotState::Completed))
        .filter(|snap| {
            if let Some(volume_id) = snap.volume_id() {
                instance_volume_ids.contains(&volume_id)
            } else {
                false
            }
        })
        .collect::<Vec<Snapshot>>();

    // Find most recent snapshot for each volume
    let mut desired_snapshots: Vec<Snapshot> = vec![];

    for block_device in instance_block_devices {
        let volume_id = get_volume_id(block_device)?;

        let desired_snapshot = instance_snapshots
            .iter()
            .find(|snap| snap.volume_id().unwrap_or_default() == volume_id)
            .cloned()
            .ok_or_else(|| {
                ApplicationError::new(format!("No snapshot found for volume {}", volume_id))
            })?;

        desired_snapshots.push(desired_snapshot);
    }

    Ok(desired_snapshots)
}

#[allow(dead_code)]
pub async fn create_volumes_from_snapshots(
    ec2_client: &Client,
    snapshots: &Vec<Snapshot>,
) -> Result<Vec<Volume>, ApplicationError> {
    // Prepare futures for creating volumes
    let mut volume_creation_futures = Vec::new();

    for snap in snapshots {
        let snapshot_id = snap
            .snapshot_id()
            .ok_or_else(|| ApplicationError::new("Snapshot should have ID"))?;

        volume_creation_futures.push(ec2_client.create_volume().snapshot_id(snapshot_id).send());
    }

    // Execute all futures and collect results
    let volume_creation_results = join_all(volume_creation_futures).await;

    // Process results and collect volume IDs
    let mut volume_ids = Vec::new();
    for result in volume_creation_results {
        match result {
            Ok(resp) => {
                let vol_id = resp.volume_id().expect("Volume should have ID");
                volume_ids.push(vol_id.into());
            }
            Err(err) => {
                return Err(ApplicationError::from_err("Error creating volume", err));
            }
        }
    }

    // Wait for volumes to become available
    let volume_creation_wait = ec2_client
        .wait_until_volume_available()
        .set_volume_ids(Some(volume_ids))
        .wait(WAIT_DURATION)
        .await
        .map_err(|err| {
            ApplicationError::from_err("Error waiting for volumes to become available", err)
        })?;

    volume_creation_wait
        .as_result()
        .map_err(|err| ApplicationError::from_err("Describe volumes error", err))
        .map(|r| r.volumes().to_owned())
}

#[allow(dead_code, unused)]
pub async fn attach_new_volumes(ec2_client: &Client, instance: &Instance) {}
