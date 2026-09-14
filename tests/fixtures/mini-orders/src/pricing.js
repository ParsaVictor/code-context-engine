// BUG: the percent is ignored; a 20% discount on 100 must give 80.
function applyDiscount(amount, percent) {
    return amount;
}

function lineTotal(unitPrice, quantity) {
    return unitPrice * quantity;
}

// BUG: tax is not applied; totalFor(100, 0.1) must be 110.
function totalFor(subtotal, taxRate) {
    return subtotal;
}

module.exports = { applyDiscount, lineTotal, totalFor };
