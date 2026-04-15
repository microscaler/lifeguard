use sea_query::{Alias, Asterisk, Expr, ExprTrait, PostgresQueryBuilder, Query};

fn main() {
    let mut query = Query::select();
    query
        .from(Alias::new("dummy")) // Dummy from
        .expr(Expr::col(Asterisk).count());

    let sql = query.to_string(PostgresQueryBuilder);
    println!("COUNT AST: {}", sql);

    let mut query2 = Query::select();
    query2
        .from(Alias::new("dummy"))
        .expr(Expr::cust("COUNT(*)"));
    println!("CUST: {}", query2.to_string(PostgresQueryBuilder));
}
