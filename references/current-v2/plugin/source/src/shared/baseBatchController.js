import { TASK_STATE } from './constants.js';

export class BaseBatchController {
  constructor() {
    this.state = TASK_STATE.IDLE;
    this.isRunning = false;
    this.isPaused = false;
    this._pauseResolve = null;
    this.onStateChange = null;
    this.collectionRunId = '';
    this.currentIndex = 0;
    this.noteList = [];
  }

  stop() {
    this._stoppedByUser = true;
    this.isRunning = false;
    this.isPaused = false;
    this._setState(TASK_STATE.STOPPING, 'stopping');
    this._onStop();
    void this._persistStoppedState();
    if (this._pauseResolve) {
      this._pauseResolve();
      this._pauseResolve = null;
    }
  }

  pause() {
    if (!this.isRunning) return;
    this.isPaused = true;
    this._setState(TASK_STATE.PAUSED, 'pause');
    void this._persistPausedState();
  }

  resume() {
    if (!this.isRunning) return;
    this.isPaused = false;
    this._setState(TASK_STATE.RUNNING, 'resume');
    void this._persistRunningState();
    if (this._pauseResolve) {
      this._pauseResolve();
      this._pauseResolve = null;
    }
  }

  async _waitIfPaused() {
    if (!this.isPaused) return;
    await new Promise((resolve) => { this._pauseResolve = resolve; });
  }

  _setState(state, phase = 'running') {
    this.state = state;
    this._emitProgress({ status: state, phase, current: this.currentIndex, total: this.noteList.length, message: '' });
  }

  _emitProgress(payload) {
    this.onStateChange?.({ taskType: this.type, taskState: this.state, ...payload });
  }

  _onStop() {}
  async _persistPausedState() {}
  async _persistRunningState() {}
  async _persistStoppedState() {}
}
