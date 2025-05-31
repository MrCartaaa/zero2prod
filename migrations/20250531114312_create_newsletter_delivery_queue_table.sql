CREATE TABLE newsletter_delivery_queue (
  newsletter_id uuid NOT NULL
  REFERENCES newsletters (newsletter_id),
  subscriber_email TEXT NOT NULL,
  PRIMARY KEY(newsletter_id, subscriber_email)
);
