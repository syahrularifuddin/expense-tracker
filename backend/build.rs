// sqlx::migrate!() menyematkan migrasi saat kompilasi; rebuild bila folder ini berubah.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
