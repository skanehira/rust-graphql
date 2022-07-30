use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{
    Context, EmptySubscription, InputObject, Object, Result, Schema, SimpleObject,
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
    title: String,
    category: String,
}

#[derive(SimpleObject, Debug)]
pub struct Wiki {
    id: String,
    owner: String,
    text: String,
    title: String,
    category: String,
}

impl From<TableWiki> for Wiki {
    fn from(item: TableWiki) -> Self {
        Wiki {
            id: item.id,
            owner: item.owner,
            text: item.text,
            title: item.title,
            category: item.category,
        }
    }
}

impl From<TableWikiPutItemOutput> for Wiki {
    fn from(item: TableWikiPutItemOutput) -> Self {
        Wiki {
            id: item.id,
            owner: item.owner,
            text: item.text,
            title: item.title,
            category: item.category,
        }
    }
}

struct Query;

#[Object]
impl Query {
    async fn wiki<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        owner: String,
        category: Option<String>,
    ) -> Result<Option<Vec<Wiki>>> {
        let dynamodb = ctx.data::<TableWikiClient>()?;

        let cond = TableWiki::key_condition(TableWiki::owner()).eq(owner);

        let mut query = dynamodb.query().index("owner").key_condition(cond);

        if let Some(category) = category {
            query = query.filter(TableWiki::filter_expression(TableWiki::category()).eq(category));
        }

        let result = query.run().await;

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

struct Mutation;

#[derive(InputObject)]
struct CreateWikiInput {
    title: String,
    owner: String,
    text: String,
    category: String,
}

#[derive(SimpleObject, Debug)]
struct DeleteWikiOutput {
    success: bool,
}

#[derive(InputObject)]
struct UpdateWikiInput {
    id: String,
    title: Option<String>,
    text: Option<String>,
    category: Option<String>,
}

#[Object]
impl Mutation {
    async fn create_wiki<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: CreateWikiInput,
    ) -> Result<Option<Wiki>> {
        let client = ctx.data::<TableWikiClient>()?;

        let input = TableWiki::put_item_builder()
            .id(xid::new().to_string())
            .title(input.title)
            .owner(input.owner)
            .text(input.text)
            .category(input.category)
            .build();

        let res = client.put(input).run().await?;
        Ok(Some(res.item.into()))
    }

    async fn delete_wiki<'ctx>(&self, ctx: &Context<'ctx>, id: String) -> Result<DeleteWikiOutput> {
        let client = ctx.data::<TableWikiClient>()?;
        match client.delete(id).run().await {
            Ok(_) => Ok(DeleteWikiOutput { success: true }),
            Err(e) => Err(e.into()),
        }
    }

    async fn update_wiki<'ctx>(
        &self,
        ctx: &Context<'ctx>,
        input: UpdateWikiInput,
    ) -> Result<Option<Wiki>> {
        let client = ctx.data::<TableWikiClient>()?;

        let mut query = client.update(input.id);

        if let Some(title) = input.title {
            let new_title = TableWiki::update_expression()
                .set(TableWiki::title())
                .value(title);
            query = query.set(new_title);
        }

        if let Some(text) = input.text {
            let new_text = TableWiki::update_expression()
                .set(TableWiki::text())
                .value(text);
            query = query.set(new_text);
        }

        if let Some(category) = input.category {
            let new_category = TableWiki::update_expression()
                .set(TableWiki::category())
                .value(category);
            query = query.set(new_category);
        }

        match query.return_all_new().run().await?.item {
            Some(output) => Ok(Some(output.into())),
            None => Ok(None),
        }
    }
}

type APISchema = Schema<Query, Mutation, EmptySubscription>;

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

    let schema = Schema::build(Query, Mutation, EmptySubscription)
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
