import { html, LitElement } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { customElement } from 'lit/decorators.js';
import { sharedStyles } from '../styles/shared.js';
import { store } from '../state/store.js';

@customElement('sv-footer')
export class SVFooter extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  static styles = sharedStyles;

  render() {
    if (!this.sphereId || !this.sphereVersion) {
      return html``;
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
      </footer>
    `;
  }
}
