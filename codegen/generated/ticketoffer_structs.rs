#[derive(Debug, Clone)]
pub struct TicketAgreement {
    pub organizer: Party,
    pub owner: Party,
}

#[derive(Debug, Clone)]
pub struct Transfer {
    pub newOwner: Party,
}

#[derive(Debug, Clone)]
pub struct Cash {
    pub issuer: Party,
    pub owner: Party,
    pub amount: Numeric,
}

#[derive(Debug, Clone)]
pub struct Accept {
    pub cashId: ContractId,
}

#[derive(Debug, Clone)]
pub struct TicketOffer {
    pub organizer: Party,
    pub buyer: Party,
    pub price: Numeric,
}

