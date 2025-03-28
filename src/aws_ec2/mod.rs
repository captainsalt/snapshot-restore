use std::{ops::Deref, time::Duration};

use aws_sdk_ec2::{
    Client,
    client::Waiters,
    types::{Filter, Instance},
};

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
    instance_names: Vec<&str>,
) -> Vec<Instance> {
    get_instances(
        ec2_client,
        Some(vec![
            Filter::builder()
                .name("tag:Name")
                .set_values(Some(instance_names.iter().map(|s| s.to_string())))
                .build(),
        ]),
    )
    .await
}

pub async fn stop_instance(ec2_client: &Client, instance: Instance) {
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

pub async fn start_instance(ec2_client: &Client, instance: Instance) {
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
