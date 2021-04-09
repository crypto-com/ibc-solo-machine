tonic::include_proto!("ibc");

use sled::Tree;
use tonic::{Request, Response, Status};

use self::ibc_server::Ibc;

pub struct IbcService {
    tree: Tree,
}

impl IbcService {
    /// Creates a new instance of ibc service
    pub fn new(tree: Tree) -> Self {
        Self { tree }
    }
}

#[tonic::async_trait]
impl Ibc for IbcService {
    async fn connect(
        &self,
        request: Request<ConnectRequest>,
    ) -> Result<Response<ConnectResponse>, Status> {
        todo!()
    }
}
