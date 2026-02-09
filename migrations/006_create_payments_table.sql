-- Create payments table -- this is where we create/update a staff from a membership
-- Primary key is helloasso_item_id because each membership can only be imported once
-- A staff can have multiple payments per season (double subscriptions are allowed)
CREATE TABLE IF NOT EXISTS payments (
    season SMALLINT NOT NULL,
    helloasso_item_id BIGINT NOT NULL,
    staff UUID NOT NULL,

    PRIMARY KEY (helloasso_item_id),
    FOREIGN KEY (helloasso_item_id) REFERENCES memberships(helloasso_item_id),
    FOREIGN KEY (staff) REFERENCES staff(id) ON DELETE RESTRICT
);
