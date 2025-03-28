use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, SdkConfig};

// TODO: Get default region from config file
// TODO: Fail if profile isn't found
// TODO: Look in multiple regions
pub async fn get_config(profile: Option<&str>) -> SdkConfig {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");

    aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .profile_name(profile.unwrap_or("default"))
        .load()
        .await
}
