pub mod app_err;
use app_err::ApplicationError;
use aws_sdk_ec2::{
    Client,
    client::Waiters,
    types::{Filter, Instance, Snapshot, SnapshotState, Tag, TagSpecification, Volume},
};
use futures::future::join_all;
use std::time::Duration;

const WAIT_DURATION: Duration = Duration::from_secs(3600); // 1 hour

/// Gets EC2 instances matching the optional filters
async fn get_instances(
    ec2_client: &Client,
    filters: Option<Vec<Filter>>,
) -> Result<Vec<Instance>, ApplicationError> {
    let response = ec2_client
        .describe_instances()
        .set_filters(filters)
        .send()
        .await
        .map_err(ApplicationError::from)?;

    Ok(response
        .reservations
        .unwrap_or_default()
        .into_iter()
        .flat_map(|reservation| reservation.instances.unwrap_or_default())
        .collect())
}

/// Finds EC2 instances by their instance IDs
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

/// Finds EC2 instances by their Name tags
pub async fn find_instances_by_name(
    ec2_client: &Client,
    instance_names: Vec<String>,
) -> Result<Vec<Instance>, ApplicationError> {
    get_instances(
        ec2_client,
        Some(vec![
            Filter::builder()
                .name("tag:Name")
                .set_values(Some(instance_names.clone()))
                .build(),
        ]),
    )
    .await
}

/// Stops an EC2 instance and waits until it's stopped
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
        .map_err(ApplicationError::from)?;

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

/// Starts an EC2 instance and waits until it's running and status checks pass
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
        .map_err(ApplicationError::from)?;

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

/// Gets all snapshots for an instance's attached volumes
pub async fn get_instance_snapshots(
    ec2_client: &Client,
    instance: &Instance,
) -> Result<Vec<Snapshot>, ApplicationError> {
    let instance_name = instance
        .tags()
        .iter()
        .find(|t| t.key() == Some("Name"))
        .ok_or_else(|| {
            ApplicationError::new(format!(
                "No Name tag found for instance {:?}",
                instance.instance_id()
            ))
        })?
        .value()
        .ok_or_else(|| {
            ApplicationError::new(format!(
                "Name tag has no value for instance {:?}",
                instance.instance_id()
            ))
        })?;

    let snapshots = ec2_client
        .describe_snapshots()
        .filters(
            Filter::builder()
                .name("tag:Name")
                .values(instance_name)
                .build(),
        )
        .send()
        .await
        .map_err(ApplicationError::from)?;

    Ok(snapshots.snapshots.unwrap_or_default())
}

/// Creates new volumes from snapshots and returns them
pub async fn create_volumes_from_snapshots(
    ec2_client: &Client,
    snapshots: &Vec<Snapshot>,
) -> Result<Vec<Volume>, ApplicationError> {
    let mut volume_futures = Vec::new();

    for snap in snapshots {
        let snapshot_id = snap
            .snapshot_id()
            .ok_or_else(|| ApplicationError::new("Snapshot should have ID"))?;

        let volume_id = snap
            .volume_id()
            .ok_or_else(|| ApplicationError::new("Volume should have ID"))?;

        // Get the device name from the original volume
        let device_name = ec2_client
            .describe_volumes()
            .volume_ids(volume_id)
            .send()
            .await
            .map_err(ApplicationError::from)?
            .volumes()
            .first()
            .ok_or_else(|| ApplicationError::new("Volume should exist"))?
            .attachments()
            .first()
            .ok_or_else(|| ApplicationError::new("Volume should be attached"))?
            .device()
            .ok_or_else(|| ApplicationError::new("Volume should have device name"))?
            .to_string();

        // Create a tag specification for the new volume
        let tag_specs = TagSpecification::builder()
            .resource_type(aws_sdk_ec2::types::ResourceType::Volume)
            .tags(Tag::builder().key("device").value(device_name).build())
            .build();

        // Create the volume
        volume_futures.push(
            ec2_client
                .create_volume()
                .tag_specifications(tag_specs)
                .snapshot_id(snapshot_id)
                .send(),
        );
    }

    // Wait for all volume creations to complete
    let volume_results = join_all(volume_futures).await;

    let volume_ids = volume_results
        .into_iter()
        .map(|result| match result {
            Ok(resp) => resp
                .volume_id()
                .map(|id| id.to_string())
                .ok_or_else(|| ApplicationError::new("Volume should have ID")),
            Err(err) => Err(ApplicationError::from_err("Error creating volume", err)),
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Wait for all volumes to become available
    let volume_wait_result = ec2_client
        .wait_until_volume_available()
        .set_volume_ids(Some(volume_ids))
        .wait(WAIT_DURATION)
        .await
        .map_err(|err| {
            ApplicationError::from_err("Error waiting for volumes to become available", err)
        })?;

    volume_wait_result
        .as_result()
        .map_err(|err| ApplicationError::from_err("Describe volumes error", err))
        .map(|r| r.volumes().to_owned())
}

/// Attaches newly created volumes to an instance
pub async fn attach_new_volumes(
    ec2_client: &Client,
    instance: &Instance,
    volumes: Vec<Volume>,
) -> Result<(), ApplicationError> {
    let instance_id = instance
        .instance_id()
        .ok_or_else(|| ApplicationError::new("Instance should have ID"))?;

    for device_mapping in instance.block_device_mappings() {
        let device_name = device_mapping
            .device_name()
            .ok_or_else(|| ApplicationError::new("Device should have name"))?;

        // Find the replacement volume with matching device tag
        let replacement_volume = volumes
            .iter()
            .find(|vol| {
                vol.tags()
                    .iter()
                    .any(|tag| tag.value() == Some(device_name))
            })
            .ok_or_else(|| {
                ApplicationError::new("Could not find volume with expected device tag")
            })?;

        let volume_id = replacement_volume
            .volume_id()
            .ok_or_else(|| ApplicationError::new("Volume should have ID"))?;

        // Detach the old volume
        ec2_client
            .detach_volume()
            .instance_id(instance_id)
            .device(device_name)
            .send()
            .await
            .map_err(ApplicationError::from)?;

        // Attach the new volume
        ec2_client
            .attach_volume()
            .instance_id(instance_id)
            .volume_id(volume_id)
            .device(device_name)
            .send()
            .await
            .map_err(ApplicationError::from)?;
    }

    Ok(())
}
