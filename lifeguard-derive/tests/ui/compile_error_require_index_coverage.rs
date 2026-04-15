//! require_index_coverage fails when a column is not covered by PK, indexed, index, or composite_unique

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "cov_test"]
#[require_index_coverage]
#[index = "idx_cov_name(name)"]
pub struct CovTest {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
}

fn main() {}
