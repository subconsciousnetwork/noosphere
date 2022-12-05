import { css, html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';
import { SphereContext } from '@subconsciousnetwork/orb';

@customElement('sv-footer')
export class SVFooter extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.fileContents')
  fileContents?: string;

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
    if (!this.sphereId || !this.sphereVersion) {
      return html``;
    }

    let downloadButton;

    if (this.fileContents?.length) {
      downloadButton = html`
        <button class="download button" @click="${() => this.downloadFile()}">
          Download this file
        </button>
      `;
    } else {
      downloadButton = html``;
    }

    return html`
      <footer class="card-footer">
        <p class="small color-secondary pad-b content">
          This sphere has been signed by its creator, and distributed P2P on
          IPFS. You can access it from any IPFS peer or gateway.
        </p>
        <ul class="group small">
          <li class="row">
            <a href="#" class="row-button">
              <div class="label">
                Sphere ID
                <span class="linkout">↗</span>
              </div>
              <div class="mono trunc color-text">${this.sphereId}</div>
            </a>
          </li>
          <li class="row">
            <a href="#" class="row-button">
              <div class="label">
                Sphere Version
                <span class="linkout">↗</span>
              </div>
              <div class="mono trunc color-text">${this.sphereVersion}</div>
            </a>
          </li>
        </ul>
        ${downloadButton}
      </footer>
    `;
  }
}
