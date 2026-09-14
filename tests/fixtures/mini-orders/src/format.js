// BUG: 12.345 must format as "12.35", not "12.34" (floor truncates).
function formatMoney(amount) {
    const cents = Math.floor(amount * 100);
    return (cents / 100).toFixed(2);
}

function formatLine(sku, quantity, total) {
    return `${sku} x${quantity} = ${formatMoney(total)}`;
}

module.exports = { formatMoney, formatLine };
