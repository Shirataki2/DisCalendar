// migrations/ が変わったら sqlx::migrate!() の埋め込みを再生成するために再ビルドさせる
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
