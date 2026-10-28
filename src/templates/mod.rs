use askama::Template;

#[derive(Template)]
#[template(path = "html/main.html")]
pub struct MainTemplate {
    pub username: String,
    pub email: String,
    pub email_verified: bool,
}

#[derive(Template)]
#[template(path = "html/_components/login_form.html")]
pub struct LoginFormTemplate {}

#[derive(Template)]
#[template(path = "html/_components/signup_form.html")]
pub struct SignupFormTemplate {
    pub invite_token: String,
    pub email: String,
}

#[derive(Template)]
#[template(path = "html/_components/forgot_form.html")]
pub struct ForgotFormTemplate {}

#[derive(Template)]
#[template(path = "html/_components/reset_form.html")]
pub struct ResetFormTemplate {
    pub token: String,
}

#[derive(Template)]
#[template(path = "html/_components/verify_form.html")]
pub struct VerifyFormTemplate {
    pub token: String,
}

#[derive(Template)]
#[template(path = "html/_components/error_message.html")]
pub struct ErrorMessageTemplate {
    pub message: String,
}

#[derive(Template)]
#[template(path = "html/_components/success_message.html")]
pub struct SuccessMessageTemplate {
    pub message: String,
}

/// Full page with a message and a single link, e.g. after following an email link
#[derive(Template)]
#[template(path = "html/_components/message_page.html")]
pub struct MessagePageTemplate {
    pub title: String,
    pub message: String,
    pub link_href: String,
    pub link_text: String,
}
