/**
 * 服务器 JSON 的运行时校验小工具（AGENTS：所有外部输入、数据库 JSON 都必须运行时校验）。
 *
 * 规则：必需字段缺失或类型不对就报「返回内容无法解析」，页面不拿残缺数据渲染；可选字段缺失是
 * `null`——未知就是未知，绝不补成 0 或空串。多出来的字段忽略，接口加字段不会让页面坏掉。
 */
import { ApiError, isRecord } from './api';

export class ShapeError extends Error {
  constructor(readonly path: string) {
    super(`unexpected shape at ${path}`);
    this.name = 'ShapeError';
  }
}

export type Json = Record<string, unknown>;

export function field(record: Json, name: string): unknown {
  if (!(name in record)) throw new ShapeError(name);
  return record[name];
}

export function obj(record: Json, name: string): Json {
  const value = field(record, name);
  if (!isRecord(value)) throw new ShapeError(name);
  return value;
}

/** 可选对象：缺失或 null 返回 null；存在但不是对象才算错。 */
export function optObj(record: Json, name: string): Json | null {
  const value = record[name];
  if (value === undefined || value === null) return null;
  if (!isRecord(value)) throw new ShapeError(name);
  return value;
}

export function str(record: Json, name: string): string {
  const value = field(record, name);
  if (typeof value !== 'string') throw new ShapeError(name);
  return value;
}

export function optStr(record: Json, name: string): string | null {
  const value = record[name];
  if (value === undefined || value === null) return null;
  if (typeof value !== 'string') throw new ShapeError(name);
  return value;
}

export function bool(record: Json, name: string): boolean {
  const value = field(record, name);
  if (typeof value !== 'boolean') throw new ShapeError(name);
  return value;
}

/** 三态布尔：null 表示「没读到」，不是 false。 */
export function optBool(record: Json, name: string): boolean | null {
  const value = record[name];
  if (value === undefined || value === null) return null;
  if (typeof value !== 'boolean') throw new ShapeError(name);
  return value;
}

export function num(record: Json, name: string): number {
  const value = field(record, name);
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new ShapeError(name);
  return value;
}

export function optNum(record: Json, name: string): number | null {
  const value = record[name];
  if (value === undefined || value === null) return null;
  if (typeof value !== 'number' || !Number.isFinite(value)) throw new ShapeError(name);
  return value;
}

export function list<T>(record: Json, name: string, parse: (item: Json, index: number) => T): T[] {
  const value = field(record, name);
  if (!Array.isArray(value)) throw new ShapeError(name);
  return value.map((item, index) => {
    if (!isRecord(item)) throw new ShapeError(`${name}[${index}]`);
    return parse(item, index);
  });
}

/** 可选的字符串数组：缺失当空数组，元素不是字符串才算错。 */
export function strList(record: Json, name: string): string[] {
  const value = record[name];
  if (value === undefined || value === null) return [];
  if (!Array.isArray(value)) throw new ShapeError(name);
  return value.map((item, index) => {
    if (typeof item !== 'string') throw new ShapeError(`${name}[${index}]`);
    return item;
  });
}

/** 运行解码；形状不对统一变成「返回内容无法解析」。 */
export function decode<T>(value: unknown, parse: (root: Json) => T): T {
  try {
    if (!isRecord(value)) throw new ShapeError('root');
    return parse(value);
  } catch (error) {
    if (error instanceof ShapeError) throw new ApiError({ kind: 'invalid_response', status: 200 });
    throw error;
  }
}
