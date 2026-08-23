fn main() {
    // migrations/ が変わったら sqlx::migrate!() の埋め込みを再生成するために再ビルドさせる
    println!("cargo:rerun-if-changed=migrations");
    // ビルド情報 (build_info.rs の option_env!) は環境変数だけが変わっても cargo からは
    // 再ビルドの必要が分からないので、変化を検知させる (#37)
    println!("cargo:rerun-if-env-changed=GIT_SHA");
    println!("cargo:rerun-if-env-changed=IMAGE_TAG");
}
