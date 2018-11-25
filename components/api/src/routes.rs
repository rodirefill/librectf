use actix_web::{App, HttpResponse};

use api::APIMiddleware;
use State;

pub fn router(state: State) -> App<State> {
    use team::{middleware::Boolean::*, TeamRequired};
    use user::LoginRequired;
    App::with_state(state)
        .middleware(APIMiddleware)
        .resource("/", |r| r.f(|_| HttpResponse::Ok().json("hello there")))
        .resource("/scoreboard", |r| r.with(self::base::scoreboard))
        .scope("/chal", |scope| {
            scope
                .middleware(TeamRequired(False))
                .resource("/list", |r| r.get().with(self::chal::list))
                .resource("/submit", |r| r.post().with(self::chal::submit))
        }).scope("/team", |scope| {
            scope
                .middleware(LoginRequired)
                .resource("/create", |r| r.post().with(self::team::create))
                .resource("/me", |r| r.get().with(self::team::me))
                .resource("/accept", |r| r.post().with(self::team::accept))
                .nested("/manage", |scope| {
                    scope
                        .middleware(TeamRequired(True))
                        .resource("/invite", |r| r.post().with(self::team::manage::invite))
                        .resource("/kick", |r| r.post().with(self::team::manage::kick))
                })
        }).scope("/user", |scope| {
            scope
                .resource("/login", |r| r.post().with(self::user::login))
                .resource("/register", |r| r.post().with(self::user::register))
        })
}

mod base {
    use actix_web::{HttpResponse, Query};
    use scoreboard::{get_scoreboard, ScoreboardOptions};
    use DbConn;

    pub fn scoreboard((query, db): (Query<ScoreboardOptions>, DbConn)) -> HttpResponse {
        get_scoreboard(db, &query.into_inner())
            .map(|entries| {
                info!("Scoreboard: {:?}", entries);
                HttpResponse::Ok().json(entries)
            }).unwrap_or_else(|err| {
                error!("Error while fetching scoreboard: {}", err);
                HttpResponse::InternalServerError().finish()
            })
    }
}

mod chal {
    use actix_web::{HttpResponse, Json};
    use chal::{list_all, submit_flag, Submission, SubmitForm};
    use DbConn;

    pub fn list(db: DbConn) -> HttpResponse {
        list_all(db)
            .map(|chals| {
                HttpResponse::Ok().json(
                    chals
                        .iter()
                        .map(|chal| {
                            json!({
                                "title": chal.title,
                                "value": chal.value,
                                "description": chal.description,
                            })
                        }).collect::<Vec<_>>(),
                )
            }).unwrap_or_else(|err| {
                error!("Error while listing chals: {}", err);
                HttpResponse::InternalServerError().finish()
            })
    }

    pub fn submit((form, db): (Json<SubmitForm>, DbConn)) -> HttpResponse {
        let form = form.into_inner();
        let submission = Submission {
            user_id: 1,
            team_id: 1,
            form,
        };
        submit_flag(db, submission)
            .map(|result| HttpResponse::Ok().json(result))
            .unwrap_or_else(|err| {
                error!("Error during submission: {}", err);
                HttpResponse::InternalServerError().finish()
            })
    }
}

mod team {
    use actix_web::{HttpRequest, HttpResponse, Json};
    use team::{create_team, my_profile, CreateTeamForm};
    use user::auth::LoginClaims;
    use {DbConn, State};

    pub fn create(
        (req, form, db): (HttpRequest<State>, Json<CreateTeamForm>, DbConn),
    ) -> HttpResponse {
        let ext = req.extensions();
        let claims = ext.get::<LoginClaims>().unwrap();
        let form = form.into_inner();
        create_team(db, claims.id, form)
            .map(|_| HttpResponse::Ok().finish())
            .unwrap_or_else(|err| {
                error!("Error during team creation: {}", err);
                HttpResponse::InternalServerError().finish()
            })
    }

    pub fn me((req, db): (HttpRequest<State>, DbConn)) -> HttpResponse {
        let ext = req.extensions();
        let claims = ext.get::<LoginClaims>().unwrap();

        my_profile(db, claims.id)
            .map(|profile| HttpResponse::Ok().json(profile))
            .unwrap_or_else(|err| {
                error!("Error fetching profile: {}", err);
                HttpResponse::InternalServerError().finish()
            })
    }

    pub fn accept(_db: DbConn) -> HttpResponse {
        // TODO: finish this
        HttpResponse::Ok().finish()
    }

    pub mod manage {
        use actix_web::{HttpResponse, Json};
        use team::manage::{invite_user, InviteUserForm};
        use DbConn;

        pub fn invite((form, db): (Json<InviteUserForm>, DbConn)) -> HttpResponse {
            let form = form.into_inner();
            invite_user(db, form)
                .map(|_| HttpResponse::Ok().finish())
                .unwrap_or_else(|err| {
                    error!("Error inviting user: {}", err);
                    HttpResponse::InternalServerError().finish()
                })
        }

        pub fn kick(_db: DbConn) -> HttpResponse {
            // TODO: finish this
            HttpResponse::Ok().finish()
        }
    }
}

mod user {
    use actix_web::{HttpRequest, HttpResponse, Json};
    use user::auth::{login_user, register_user, LoginForm, RegisterForm, UserError};
    use {DbConn, State};

    pub fn login((req, form, db): (HttpRequest<State>, Json<LoginForm>, DbConn)) -> HttpResponse {
        let state = req.state();
        let form = form.into_inner();

        info!("Login request: email={:?}", form.email);
        login_user(db, state.get_secret_key(), form)
            .map(|(user, token)| {
                info!(
                    "Successfully logged in: id={:?}, email={:?}",
                    user.id, user.email
                );
                HttpResponse::Ok().json(token)
            }).unwrap_or_else(|err| match err {
                UserError::AlreadyRegistered => HttpResponse::BadRequest().finish(),
                UserError::BadUsernameOrPassword => HttpResponse::Unauthorized().finish(),
                UserError::ServerError(err) => {
                    error!("Error logging in: {}", err);
                    HttpResponse::InternalServerError().finish()
                }
            })
    }

    pub fn register(
        (req, form, db): (HttpRequest<State>, Json<RegisterForm>, DbConn),
    ) -> HttpResponse {
        let state = req.state();
        let form = form.into_inner();
        info!(
            "Register request: username={:?}, email={:?}",
            form.username, form.email
        );
        register_user(db, state.get_secret_key(), form)
            .map(|(user, token)| {
                info!(
                    "Successfully registered: id={:?}, username={:?}",
                    user.id, user.name
                );
                HttpResponse::Ok().json(token)
            }).unwrap_or_else(|err| {
                error!("Error registering: {}", err);
                HttpResponse::InternalServerError().finish()
            })
    }
}
