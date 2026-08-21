export interface PaperAccount { id: string; name: string; base_currency: string; cash_balance: string; status: string; created_at: string; updated_at: string }
export interface PaperTrade { id: string; property_id: string; side: string; units: string; execution_price: string; gross_amount: string; currency: string; executed_at: string }
