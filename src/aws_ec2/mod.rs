#![allow(dead_code)]

use aws_sdk_ec2::{
    Client,
    client::Waiters,
    error::SdkError,
    operation::describe_snapshots::DescribeSnapshotsError,
    types::{Filter, Instance, InstanceBlockDeviceMapping, Snapshot, SnapshotState, Tag},
};
use futures::future::join_all;
use std::time::Duration;

fn get_tag_value<'a>(tags: &'a Vec<Tag>, value: &str) -> Option<&'a Tag> {
    tags.iter()
        .find(|tag| tag.value().unwrap_or_default() == value)
}

async fn get_instances(ec2_client: &Client, filters: Option<Vec<Filter>>) -> Vec<Instance> {
    ec2_client
        .describe_instances()
        .set_filters(filters)
        .send()
        .await
        .expect("Failed to describe instances")
        .reservations
        .unwrap_or_default()
        .into_iter()
        .flat_map(|reservation| reservation.instances.unwrap_or_default())
        .collect()
}

pub async fn find_instances_by_id(ec2_client: &Client, instance_ids: Vec<String>) -> Vec<Instance> {
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
) -> Vec<Instance> {
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

pub async fn stop_instance(ec2_client: &Client, instance: &Instance) {
    ec2_client
        .stop_instances()
        .instance_ids(instance.instance_id.as_deref().unwrap_or_default())
        .send()
        .await
        .expect("Error stopping instances");

    ec2_client
        .wait_until_instance_stopped()
        .instance_ids(instance.instance_id.as_deref().unwrap_or_default())
        .wait(Duration::from_secs(600_000))
        .await
        .expect("Error waiting for instances to stop");
}

pub async fn start_instance(ec2_client: &Client, instance: &Instance) {
    ec2_client
        .start_instances()
        .instance_ids(instance.instance_id.as_deref().unwrap_or_default())
        .send()
        .await
        .expect("Error starting instances");

    ec2_client
        .wait_until_instance_status_ok()
        .instance_ids(instance.instance_id.as_deref().unwrap_or_default())
        .wait(Duration::from_secs(600_000))
        .await
        .expect("Error waiting for instances to start");
}

pub async fn get_instance_snapshots(
    ec2_client: &Client,
    instance: &Instance,
) -> Result<Option<Vec<Snapshot>>, SdkError<DescribeSnapshotsError>> {
    let volume_ids = instance
        .block_device_mappings()
        .iter()
        .map(|device| {
            device
                .ebs()
                .expect("Instance should have EBS volume attached")
                .volume_id()
                .expect("Volume should have ID")
                .to_string()
        })
        .collect::<Vec<_>>();

    let snapshots = ec2_client
        .describe_snapshots()
        .filters(
            Filter::builder()
                .name("volume-id")
                .set_values(Some(volume_ids))
                .build(),
        )
        .send()
        .await?;

    Ok(snapshots.snapshots)
}

pub async fn get_most_recent_snapshots<'a>(
    instance: &'a Instance,
    snapshots: &Vec<Snapshot>,
) -> Vec<Snapshot> {
    let instance_block_devices = instance.block_device_mappings();
    let mut snapshots: Vec<Snapshot> = snapshots.to_owned();

    snapshots.sort_by(|a, b| {
        let a_time = a.start_time().expect("Snapshot should have a start time");
        let b_time = b.start_time().expect("Snapshot should have a start time");
        b_time.cmp(&a_time)
    });

    let get_volume_id = |device_mapping: &'a InstanceBlockDeviceMapping| {
        device_mapping
            .ebs()
            .expect("EBS Should exist")
            .volume_id()
            .expect("Volume ID should exist if EBS exists")
    };

    let instance_snapshots = snapshots
        .into_iter()
        .filter(|snap| snap.state() == Some(&SnapshotState::Completed))
        .filter(|snap| {
            let instance_volume_ids = instance_block_devices
                .iter()
                .map(get_volume_id)
                .collect::<Vec<_>>();

            instance_volume_ids.contains(&snap.volume_id().unwrap())
        })
        .collect::<Vec<Snapshot>>();

    let mut desired_snapshots: Vec<Snapshot> = vec![];

    for block_device in instance_block_devices {
        let desired_snapshot = instance_snapshots
            .iter()
            .find(|snap| {
                snap.volume_id().unwrap_or_default() == get_volume_id(block_device).to_string()
            })
            .cloned();

        desired_snapshots.push(desired_snapshot.unwrap());
    }

    desired_snapshots
}

pub async fn create_volumes_from_snapshots(ec2_client: &Client, snapshots: &Vec<Snapshot>) {
    let snapshot_futures = snapshots
        .iter()
        .map(|snap| {
            ec2_client
                .create_volume()
                .snapshot_id(snap.snapshot_id().expect("Snapshot should have ID"))
                .send()
        })
        .collect::<Vec<_>>();

    let snapshot_results = join_all(snapshot_futures).await;
    let has_errors = snapshot_results.iter().any(Result::is_err);

    if has_errors {
        panic!("Error creating snapshots");
    }

    let volume_ids: Vec<String> = snapshot_results
        .iter()
        .map(|r| r.as_ref().unwrap().volume_id.clone().unwrap())
        .collect();

    let _ = ec2_client
        .wait_until_volume_available()
        .set_volume_ids(Some(volume_ids))
        .wait(Duration::from_secs(600_000))
        .await;
}
