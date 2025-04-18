/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { CalculatorRequest } from '../models/CalculatorRequest';
import type { CalculatorResponse } from '../models/CalculatorResponse';
import type { CancelablePromise } from '../core/CancelablePromise';
import type { BaseHttpRequest } from '../core/BaseHttpRequest';
export class Adder {
    constructor(public readonly httpRequest: BaseHttpRequest) {}
    /**
     * @param requestBody
     * @returns CalculatorResponse
     * @throws ApiError
     */
    public postAdd(
        requestBody: CalculatorRequest,
    ): CancelablePromise<CalculatorResponse> {
        return this.httpRequest.request({
            method: 'POST',
            url: '/add',
            body: requestBody,
            mediaType: 'application/json; charset=utf-8',
        });
    }
}
