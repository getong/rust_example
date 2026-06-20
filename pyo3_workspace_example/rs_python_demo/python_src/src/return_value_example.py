def calculate_invoice(
    quantity: int,
    unit_price: float,
    discount_rate: float,
) -> tuple[float, float, float]:
    subtotal = quantity * unit_price
    discount_amount = subtotal * discount_rate
    total = subtotal - discount_amount
    return subtotal, discount_amount, total
