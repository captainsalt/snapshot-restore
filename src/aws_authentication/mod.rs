use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, SdkConfig};

pub async fn get_profile(profile: Option<&str>) -> SdkConfig {
    let region_provider = RegionProviderChain::default_provider();

    aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .profile_name(profile.unwrap_or("default"))
        .load()
        .await
}
