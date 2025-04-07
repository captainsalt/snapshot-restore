use crate::app_err::ApplicationError;
use aws_sdk_ec2::{
    Client,
    types::{Instance, Snapshot},
};
use inquire::Select;

fn snapshot_to_string(snap: &Snapshot) -> String {
    format!(
        "{} {} {} {} GiB",
        snap.start_time().unwrap(),
        snap.tags()
            .iter()
            .find(|t| t.key() == Some("Name"))
            .unwrap()
            .value()
            .unwrap_or("<NO NAME>"),
        snap.snapshot_id().unwrap(),
        snap.volume_size().unwrap()
    )
}

pub async fn pick_snapshots(
    ec2_client: &Client,
    instance: &Instance,
    snapshots: &[Snapshot],
) -> Result<Vec<Snapshot>, ApplicationError> {
    let mut snapshot_selections = Vec::new();

    for device in instance.block_device_mappings() {
        let volume_id = device
            .ebs()
            .expect("EBS should exist")
            .volume_id()
            .expect("Volume should have ID");

        let volume_size = ec2_client
            .describe_volumes()
            .volume_ids(volume_id)
            .send()
            .await
            .map_err(ApplicationError::from)?
            .volumes()
            .first()
            .expect("Volume should exist")
            .size();

        let matching_snapshots = snapshots
            .iter()
            .filter(|snapshot| snapshot.volume_size() == volume_size)
            .map(snapshot_to_string)
            .collect::<Vec<_>>();

        let select_prompt = format!(
            "Please select snapshot to restore to {}",
            device.device_name().unwrap()
        );
        let snapshot_selection = Select::new(&select_prompt, matching_snapshots).prompt();

        match snapshot_selection {
            Ok(snapshot_string) => {
                let selected_snapshot = snapshots
                    .iter()
                    .find(|s| snapshot_to_string(s) == snapshot_string)
                    .unwrap();

                snapshot_selections.push(selected_snapshot.to_owned())
            }

            Err(error) => panic!("{:?}", error),
        }
    }

    Ok(snapshot_selections)
}
