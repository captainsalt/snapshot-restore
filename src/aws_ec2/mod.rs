#![allow(dead_code)]

use aws_sdk_ec2::{
    Client,
    client::Waiters,
    types::{Filter, Instance, Snapshot, SnapshotState, Tag},
};
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

pub async fn get_instance_snapshots(ec2_client: &Client, instance: &Instance) -> Vec<Snapshot> {
    let volumes = instance
        .block_device_mappings
        .as_ref()
        .expect("Instance should have block devices attached");

    for volume in volumes {
        print!(
            "{} -> {}",
            volume.device_name.as_ref().expect("Device should exist"),
            volume
                .ebs
                .as_ref()
                .expect("EBS volume should exist")
                .volume_id
                .as_ref()
                .expect("Volume ID should exist")
        )
    }

    let instance_name = instance
        .tags
        .as_ref()
        .and_then(|tags| tags.iter().find(|t| t.key.as_ref().unwrap() == "Name"))
        .expect("Name tag should exist on instance")
        .value
        .as_ref()
        .expect("Name tag should have a value");

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
        .expect("Failed to describe snapshots")
        .snapshots
        .unwrap_or_default();

    snapshots
}

pub async fn get_most_recent_snapshot(
    instance: &Instance,
    snapshots: &Vec<Snapshot>,
) -> Vec<Snapshot> {
    let instance_block_devices = instance.block_device_mappings();
    let mut snapshots: Vec<Snapshot> = snapshots.to_owned();

    snapshots.sort_by(|a, b| {
        let a_time = a.start_time().expect("Snapshot should have a start time");
        let b_time = b.start_time().expect("Snapshot should have a start time");
        b_time.cmp(&a_time)
    });

    let instance_snapshots = snapshots
        .into_iter()
        .filter(|snap| snap.state() == Some(&SnapshotState::Completed))
        .filter(|snap| {
            let instance_volume_ids = instance_block_devices
                .iter()
                .map(|b| b.ebs().unwrap().volume_id().unwrap())
                .collect::<Vec<_>>();

            instance_volume_ids.contains(&snap.volume_id().unwrap())
        })
        .collect::<Vec<Snapshot>>();

    let mut desired_snapshots: Vec<Snapshot> = vec![];

    for block_device in instance_block_devices {
        let desired_snapshot = instance_snapshots
            .iter()
            .find(|snap| {
                snap.volume_id == block_device.ebs().unwrap().volume_id().map(String::from)
            })
            .cloned();

        desired_snapshots.push(desired_snapshot.unwrap());
    }

    desired_snapshots
}
