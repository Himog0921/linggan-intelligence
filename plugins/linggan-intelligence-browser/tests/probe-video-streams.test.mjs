import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';

const probeSource = fs.readFileSync(
  path.resolve('scripts/probe-video-streams.js'),
  'utf8',
);

test('video stream probe reports verified opaque XHS stream groups', () => {
  const context = {
    window: {
      __INITIAL_STATE__: {
        note: {
          noteDetailMap: {
            '6a69ef3f000000000f030555': {
              note: {
                noteId: '6a69ef3f000000000f030555',
                video: {
                  media: {
                    stream: {
                      EF4: [{
                        masterUrl: 'https://sns-video.example/ef4-master.mp4',
                        backupUrls: ['https://sns-video.example/ef4-backup.mp4'],
                        avgBitrate: 1200,
                        width: 720,
                        height: 1280,
                      }],
                      EF5: [{
                        masterUrl: 'https://sns-video.example/ef5-master.mp4',
                        avgBitrate: 2600,
                        width: 1080,
                        height: 1920,
                      }],
                      EF6: [],
                      metadata: { revision: 1 },
                    },
                  },
                },
              },
            },
          },
        },
      },
    },
    location: { href: 'https://www.xiaohongshu.com/explore/6a69ef3f000000000f030555' },
    console: { log() {}, table() {}, warn() {} },
  };

  const output = vm.runInNewContext(probeSource, context);

  assert.equal(output.streamCount, 2);
  assert.deepEqual(
    JSON.parse(JSON.stringify(output.groupCounts)),
    { EF4: 1, EF5: 1, EF6: 0 },
  );
  assert.deepEqual(
    Array.from(output.rows, (row) => row.group),
    ['EF4', 'EF5'],
  );
  assert.equal(output.rows[1].url, 'https://sns-video.example/ef5-master.mp4');
});
