
(function () {
  'use strict';

  function findMarker(bytes, first, second, start) {
    for (let i = start; i < bytes.length - 1; i++) {
      if (bytes[i] === first && bytes[i + 1] === second) {
        return i;
      }
    }
    return -1;
  }

  async function playCamera(canvas) {
    if (canvas.dataset.running === '1') return;

    const token = sessionStorage.getItem('smartsecure_token');
    if (!token) return;

    canvas.dataset.running = '1';
    const controller = new AbortController();
    let reader;
    let buffer = new Uint8Array(0);

    try {
      const response = await fetch(
        '/api/camera/' + canvas.dataset.cameraLive + '/stream',
        {
          headers: {
            Authorization: 'Bearer ' + token
          },
          cache: 'no-store',
          signal: controller.signal
        }
      );

      if (!response.ok || !response.body) {
        throw new Error('Camera HTTP ' + response.status);
      }

      reader = response.body.getReader();
      const ctx = canvas.getContext('2d');
      const page = document.querySelector('[data-page="monitor"]');

      while (
        canvas.isConnected &&
        !document.hidden &&
        page &&
        !page.hidden
      ) {
        const result = await reader.read();
        if (result.done) break;

        const chunk = result.value;
        const combined = new Uint8Array(buffer.length + chunk.length);
        combined.set(buffer);
        combined.set(chunk, buffer.length);
        buffer = combined;

        if (buffer.length > 4 * 1024 * 1024) {
          buffer = new Uint8Array(0);
          continue;
        }

        while (true) {
          const start = findMarker(buffer, 255, 216, 0);

          if (start < 0) {
            buffer = new Uint8Array(0);
            break;
          }

          const end = findMarker(buffer, 255, 217, start + 2);

          if (end < 0) {
            buffer = buffer.slice(start);
            break;
          }

          const jpg = buffer.slice(start, end + 2);
          buffer = buffer.slice(end + 2);

          try {
            const image = await createImageBitmap(
              new Blob([jpg], { type: 'image/jpeg' })
            );

            if (!canvas.isConnected) {
              image.close();
              break;
            }

            ctx.drawImage(
              image,
              0,
              0,
              canvas.width,
              canvas.height
            );

            image.close();
          } catch (error) {
            console.warn('Không giải mã được khung hình:', error);
          }
        }
      }
    } catch (error) {
      if (error.name !== 'AbortError') {
        console.warn('Camera LIVE:', error.message);
      }
    } finally {
      controller.abort();

      if (reader) {
        try {
          await reader.cancel();
        } catch (_) {}
      }

      canvas.dataset.running = '0';
    }
  }

  setInterval(function () {
    const page = document.querySelector('[data-page="monitor"]');

    if (!page || page.hidden || document.hidden) return;

    document.querySelectorAll(
      'canvas[data-camera-live]'
    ).forEach(playCamera);
  }, 2000);
})();
