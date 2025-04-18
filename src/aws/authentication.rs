use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region, SdkConfig};

pub async fn get_profile(profile: Option<String>, region: Option<String>) -> SdkConfig {
    if profile.is_none() {
        panic!("Profile not specified")
    }

    let default_region = RegionProviderChain::default_provider().region().await;
    let provided_region = region.map(Region::new).clone();
    let region = match (provided_region, default_region) {
        (Some(provided_region), _) => provided_region,
        (_, Some(default_region)) => default_region,
        (None, None) => panic!("No regions found"),
    };

    aws_config::defaults(BehaviorVersion::latest())
        .region(region)
        .profile_name(profile.unwrap())
        .load()
        .await
}
