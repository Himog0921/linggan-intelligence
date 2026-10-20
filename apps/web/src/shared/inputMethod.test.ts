import { fireEvent } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { trackInputMethod } from './inputMethod';

describe('C焦点输入方式',()=>{
 it('鼠标点击文本框仍判定为鼠标；Tab和输入改为键盘，不拦截原事件',()=>{
  const cleanup=trackInputMethod(document);const input=document.createElement('input');document.body.append(input);
  fireEvent.pointerDown(input);expect(document.documentElement.dataset.inputMode).toBe('pointer');
  fireEvent.keyDown(input,{key:'Meta',metaKey:true});expect(document.documentElement.dataset.inputMode).toBe('pointer');
  fireEvent.keyDown(input,{key:'Tab'});expect(document.documentElement.dataset.inputMode).toBe('keyboard');
  fireEvent.pointerDown(input);fireEvent.keyDown(input,{key:'a'});expect(document.documentElement.dataset.inputMode).toBe('keyboard');
  cleanup();input.remove();
 });
});
