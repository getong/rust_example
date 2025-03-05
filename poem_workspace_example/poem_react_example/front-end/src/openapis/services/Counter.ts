/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { CalculatorResponse } from '../models/CalculatorResponse';
import type { CancelablePromise } from '../core/CancelablePromise';
import type { BaseHttpRequest } from '../core/BaseHttpRequest';
export class Counter {
    constructor(public readonly httpRequest: BaseHttpRequest) {}
    /**
     * @returns CalculatorResponse
     * @throws ApiError
     */
    public getCounter(): CancelablePromise<CalculatorResponse> {
        return this.httpRequest.request({
            method: 'GET',
            url: '/counter',
        });
    }
}
