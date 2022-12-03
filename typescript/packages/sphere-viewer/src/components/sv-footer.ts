import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';
import { SphereContext } from '@subconsciousnetwork/orb';
import { until } from 'lit/directives/until.js';

@customElement('sv-footer')
export class SVFooter extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.fileContents')
  fileContents?: string;

  @watch('sphereViewer.fileVersion')
  fileVersion?: string;

  @watch('sphereViewer.sphere')
  sphere?: SphereContext;

  @watch('sphereViewer.slug')
  slug?: string;

  static styles = [
    sharedStyles,
    css`
      .download {
        display: block;
        margin-top: 1em;
        width: 100%;
      }
    `,
  ];

  async downloadFile() {
    const sphere = this.sphere;
    const slug = this.slug;

    if (!sphere || !slug) {
      return;
    }

    const fs = await sphere.fsAt(this.sphereVersion!);
    const file = await fs.read(slug);
    const contentType = file?.contentType();
    const bytes = await file?.intoBytes();

    if (!bytes) {
      return;
    }

    const blob = new Blob([bytes], {
      type: contentType,
    });

    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');

    let extension;

    switch (contentType) {
      case 'text/subtext':
        extension = '.subtext';
        break;
      case 'text/plain':
        extension = '.txt';
        break;
      case 'text/markdown':
        extension = '.md';
        break;
      default:
        extension = '';
        break;
    }

    anchor.href = url;
    anchor.setAttribute('download', `${slug}${extension}`);
    anchor.click();
  }

  render() {
    let fileContentsResolve = Promise.resolve(this.fileContents);

    const hashRow = until(
      fileContentsResolve.then((contents) => {
        return contents
          ? html`
              <li class="row">
                <div class="pad-row">
                  <div class="label">File version</div>
                  <div class="mono trunc color-text">${this.fileVersion}</div>
                </div>
              </li>
            `
          : html``;
      }),
      html``
    );

    const downloadButton = until(
      fileContentsResolve.then((contents) => {
        return contents
          ? html`
              <button
                class="download button"
                @click="${() => this.downloadFile()}"
              >
                Download this file
              </button>
            `
          : html``;
      })
    );

    return html`
      <footer class="card-footer">
        <p class="small color-secondary pad-b content">
          This sphere has been signed by its creator, and distributed P2P on
          IPFS. You can access it from any IPFS peer or gateway.
        </p>
        <ul class="group small">
          ${hashRow}

          <li class="row">
            <!-- TODO: Enable this when we discover a good place to view hashes
            <!-- <a
              href="https://explore.ipld.io/#/explore/${this.sphereVersion}"
              class="row-button"
              target="_blank"
              rel="noopener"
            > -->
            <div class="pad-row">
              <div class="label">Sphere Version</div>
              <div class="mono trunc color-text">${this.sphereVersion}</div>
            </div>
            <!-- </a> -->
          </li>
          <li class="row">
            <div class="pad-row">
              <div class="label">Sphere ID</div>
              <div class="mono trunc color-text">${this.sphereId}</div>
            </div>
          </li>
        </ul>
        ${downloadButton}
      </footer>
    `;
  }
}
