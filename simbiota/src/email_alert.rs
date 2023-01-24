use crate::daemon_config::{DaemonConfig, SmtpConnectionSecurity};
use libc::fanotify_event_metadata;
use log::{debug, info, warn};
use std::rc::Rc;
use std::sync::Arc;

use crate::detection_system::DetectionDetails;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

pub struct EmailAlertSystem {
    config: Arc<DaemonConfig>,
}

impl EmailAlertSystem {
    pub fn new(config: Arc<DaemonConfig>) -> Self {
        Self { config }
    }

    pub fn send_email_alert(&self, data: &DetectionDetails) {
        info!("sending email notification");

        let mut email = Message::builder().from(
            format!(
                "SIMBIoTA AV <{}>",
                self.config.email.smtp_config.as_ref().unwrap().username
            )
            .parse()
            .unwrap(),
        );
        for recp in &self.config.email.recipients {
            email = email.to(recp.parse().unwrap());
        }

        let email = email
            .subject("SIMBIoTA Alert")
            .body(self.gen_body(data))
            .unwrap();

        let smtp_config = self.config.email.smtp_config.as_ref().unwrap();
        let creds = Credentials::new(
            smtp_config.username.clone(),
            smtp_config.password.clone().unwrap_or("".to_string()),
        );

        let mailer = match smtp_config.security {
            SmtpConnectionSecurity::None => SmtpTransport::builder_dangerous(&smtp_config.server)
                .port(smtp_config.port)
                .credentials(creds)
                .build(),
            SmtpConnectionSecurity::Ssl => SmtpTransport::relay(&smtp_config.server)
                .unwrap()
                .port(smtp_config.port)
                .credentials(creds)
                .build(),
            SmtpConnectionSecurity::Starttls => SmtpTransport::starttls_relay(&smtp_config.server)
                .unwrap()
                .port(smtp_config.port)
                .credentials(creds)
                .build(),
        };

        debug!("sending email using {:?}", smtp_config.server);
        let result = mailer.send(&email);
        if result.is_err() {
            warn!("failed to send email: {}", result.unwrap_err());
        } else {
            debug!("alert email sent");
        }
    }

    fn gen_body(&self, data: &DetectionDetails) -> String {
        format!(
            r#"SIMBIoTA Alert message: 
        
        The system detected a malicious file: {}
        Detection time: {}"#,
            data.path, data.time
        )
    }
}
