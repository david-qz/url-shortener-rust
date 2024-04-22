use core::fmt;

use aws_sdk_dynamodb::types::AttributeValue;
use nanoid::nanoid;

#[derive(Debug)]
pub enum Error {
    NotFound,
    ServerError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::NotFound => write!(f, "No link found for key"),
            Error::ServerError => write!(f, "Internal server error"),
        }
    }
}

impl<E> From<aws_sdk_dynamodb::error::SdkError<E>> for Error {
    fn from(_: aws_sdk_dynamodb::error::SdkError<E>) -> Self {
        Error::ServerError
    }
}

#[derive(Clone)]
pub struct LinkStorage {
    dynamodb_client: aws_sdk_dynamodb::Client,
    table_name: String,
}

impl LinkStorage {
    pub fn new(dynamodb_client: aws_sdk_dynamodb::Client, table_name: String) -> Self {
        Self {
            dynamodb_client,
            table_name,
        }
    }

    pub async fn get_link_for_key(&self, key: String) -> Result<String, Error> {
        let resp = self
            .dynamodb_client
            .get_item()
            .table_name(&self.table_name)
            .key("key", AttributeValue::S(key))
            .projection_expression("#u")
            .expression_attribute_names("#u", "url")
            .send()
            .await?;

        match resp.item {
            Some(item) => {
                let url = item.get("url").and_then(|attr| attr.as_s().ok());
                match url {
                    Some(url) => Ok(url.into()),
                    None => Err(Error::NotFound),
                }
            }
            None => Err(Error::NotFound),
        }
    }

    pub async fn create_short_link(&self, url: String) -> Result<String, Error> {
        let key = nanoid!(10, &ALPHABET);

        self.dynamodb_client
            .put_item()
            .table_name(&self.table_name)
            .item("key", AttributeValue::S(key.clone()))
            .item("url", AttributeValue::S(url))
            .send()
            .await?;

        Ok(key)
    }
}

const ALPHABET: [char; 56] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'J', 'K', 'L',
    'M', 'N', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f',
    'g', 'h', 'i', 'j', 'k', 'm', 'n', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
