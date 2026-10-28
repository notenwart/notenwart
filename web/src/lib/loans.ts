// SPDX-License-Identifier: AGPL-3.0-or-later
import { api, type Loan } from './api/index.js';

/** Today as YYYY-MM-DD in the device's time zone. */
export function today(now = new Date()): string {
	const two = (n: number) => String(n).padStart(2, '0');
	return `${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}`;
}

/** Not returned and past its due date. */
export function overdue(loan: Pick<Loan, 'dueOn' | 'returnedOn'>, on = today()): boolean {
	return !loan.returnedOn && !!loan.dueOn && loan.dueOn < on;
}

/** Marks a loan returned today. */
export function returnLoan(loan: Loan) {
	return api.PUT('/loans/{loanId}', {
		params: { path: { loanId: loan.id } },
		body: {
			item: loan.item,
			quantity: loan.quantity,
			borrowerId: loan.borrowerId,
			borrowerName: loan.borrowerId ? null : loan.borrowerName,
			lentOn: loan.lentOn,
			dueOn: loan.dueOn,
			returnedOn: today(),
			note: loan.note
		}
	});
}
