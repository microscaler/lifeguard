fn main() {
    use sea_query::{Alias, BinOper, Expr, ExprTrait, PostgresQueryBuilder, Query};
    let mut q = Query::select();
    q.column(sea_query::Asterisk);
    q.from(Alias::new("table"));

    let expr = Expr::col((Alias::new("table"), Alias::new("tags_json"))).binary(
        BinOper::Custom("@>"),
        Expr::val("{\"label\": \"test\"}").cast_as(Alias::new("jsonb")),
    );

    q.and_where(expr);
    let (sql, values) = q.build(PostgresQueryBuilder);
    println!("SQL: {}", sql);
    println!("Values: {:?}", values);
}
