use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    *,
};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::routing::get;
use axum::{extract::Extension, Router};
use axum::{
    response::{self, IntoResponse},
    Server,
};
use raiden::*;

#[derive(Raiden, Debug)]
#[raiden(table_name = "wiki")]
pub struct TableWiki {
    #[raiden(partition_key)]
    id: String,
    owner: String,
    text: String,
    category: String,
}

#[derive(SimpleObject, Debug)]
pub struct Wiki {
    id: String,
    owner: String,
    text: String,
    category: String,
}

impl From<TableWiki> for Wiki {
    fn from(item: TableWiki) -> Self {
        Wiki {
            id: item.id,
            owner: item.owner,
            text: item.text,
            category: item.category,
        }
    }
}

struct Query;

#[Object]
impl Query {
    async fn wiki<'ctx>(&self, ctx: &Context<'ctx>, owner: String) -> Result<Option<Vec<Wiki>>> {
        let dynamodb = ctx.data::<TableWikiClient>()?;

        let cond = TableWiki::key_condition(TableWiki::owner()).eq(owner);
        let result: Result<raiden::query::QueryOutput<TableWiki>, raiden::RaidenError> = dynamodb
            .query()
            .index("owner")
            .key_condition(cond)
            .run()
            .await;

        match result {
            Ok(output) => {
                let mut wikis = Vec::<Wiki>::new();
                for item in output.items {
                    wikis.push(item.into());
                }
                Ok(Some(wikis))
            }
            Err(e) => Err(e.into()),
        }
    }
}

type APISchema = Schema<Query, EmptyMutation, EmptySubscription>;

async fn handler(schema: Extension<APISchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn playground() -> impl IntoResponse {
    response::Html(playground_source(GraphQLPlaygroundConfig::new("/")))
}

#[tokio::main]
async fn main() {
    let client = TableWiki::client(Region::Custom {
        name: "ap-northeast-1".into(),
        endpoint: "http://localhost:8001".into(),
    });

    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(client)
        .finish();

    let app = Router::new()
        .route("/", get(playground).post(handler))
        .layer(Extension(schema));
    println!("servrer listen on 0.0.0.0:8000");

    Server::bind(&"0.0.0.0:8000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
