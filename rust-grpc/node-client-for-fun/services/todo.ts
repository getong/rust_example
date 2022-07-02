import { todoClient } from '../grpc-client';
import { promisify } from 'node:util';
import { GetTodosResponse, CreateTodo } from '../proto/types';

export const getTodos: (_: unknown) => GetTodosResponse = promisify(todoClient.getTodos.bind(todoClient));
export const createTodo: CreateTodo = promisify(todoClient.createTodo.bind(todoClient));