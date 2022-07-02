use crate::database::connect_to_db::MongoDB;
use crate::database::FindUserBy;
use crate::models::model_user::User;
use bcrypt::hash;
use mongodb::bson::oid::ObjectId;
use rocket::http::Status;
use rocket::State;

//check valid text
pub fn check_valid_text(text: &str, max_size: usize, min_size: usize) -> bool {
    return if !text.is_empty() && text.len() <= max_size && text.len() >= min_size {
        true
    } else {
        false
    };
}

//check valid name
pub fn check_valid_name(text: &str, max_size: usize, min_size: usize) -> bool {
    return if text.is_empty() || text.len() <= max_size && text.len() >= min_size {
        true
    } else {
        false
    };
}

//hash text
pub fn hash_text(text: String, cost: u32) -> Result<String, Status> {
    return match hash(text, cost) {
        Ok(hash_text) => Ok(hash_text),
        Err(_) => Err(Status::BadRequest),
    };
}

//parse str to objectId
pub fn object_id_parse_str(id_str: String) -> Result<ObjectId, String> {
    match ObjectId::parse_str(id_str) {
        Ok(to_id) => Ok(to_id),
        Err(error) => Err(format!("{}", error)),
    }
}

/// find user by login and mail
pub async fn find_user_by_login_and_mail(
    database: &MongoDB,
    mail: &str,
    login: &str,
) -> FindUserBy {
    match database.find_user_by("login", login).await {
        Ok(None) => match database.find_user_by("mail", mail).await {
            Ok(None) => FindUserBy::UserNotFound,
            Ok(Some(_)) => FindUserBy::UserFoundByEmail,
            Err(_) => FindUserBy::UserFoundByEmail,
        },
        Ok(Some(_)) => FindUserBy::UserFoundByLogin,
        Err(_) => FindUserBy::UserFoundByLogin,
    }
}

//check data from request auth
pub fn check_data_from_auth_header(auth_header: Option<&str>) -> Result<Vec<&str>, ()> {
    return if let Some(auth_string) = auth_header {
        let vec_header = auth_string.split_whitespace().collect::<Vec<_>>();
        if vec_header.len() != 2
            && vec_header[0] == "Bearer"
            && !vec_header[0].is_empty()
            && !vec_header[1].is_empty()
        {
            Err(())
        } else {
            Ok(vec_header)
        }
    } else {
        Err(())
    };
}

pub enum FindUserById {
    Ok(User),
    NoneUser,
    BadId,
}

pub async fn parse_id_and_find_user_by_id(
    database: &State<MongoDB>,
    id_str: String,
) -> FindUserById {
    match object_id_parse_str(id_str) {
        Ok(id) => match database.find_user_by_id(id).await {
            Ok(Some(user)) => FindUserById::Ok(user),
            Ok(None) => FindUserById::NoneUser,
            Err(_) => FindUserById::BadId,
        },
        Err(_) => FindUserById::BadId,
    }
}
