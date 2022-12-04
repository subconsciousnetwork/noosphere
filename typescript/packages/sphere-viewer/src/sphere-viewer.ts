import { customElement } from 'lit/decorators.js';
import { LitElement, html } from 'lit';
import { connect, watch } from 'lit-redux-watch';
import { store } from './state/store.js';

@customElement('sphere-viewer')
export class SphereViewer extends connect(store)(LitElement) {
  @watch('sphereViewer.sphereId')
  sphereId?: string;

  @watch('sphereViewer.sphereVersion')
  sphereVersion?: string;

  @watch('sphereViewer.slug')
  slug?: string;

  @watch('sphereViewer.fileContents')
  fileContents?: string;

  render() {
    const metadata = [];

    if (this.sphereId != null) {
      metadata.push(html`<span>Sphere ID</span> <span>${this.sphereId}</span>`);
    }

    if (this.sphereVersion != null) {
      metadata.push(
        html`<span>Sphere Version</span> <span>${this.sphereVersion}</span>`
      );
    }

    const footer = html`<footer>
      <ul>
        ${metadata.map((item) => html`<li>${item}</li>`)}
      </ul>
    </footer>`;

    const bodyContent = [];

    if (this.slug != null) {
      bodyContent.push(html`<nav>/${this.slug}</nav>`);
    }

    if (this.fileContents != null) {
      bodyContent.push(html`<section>${this.fileContents}</section>`);
    }

    const body = html`<section role="main">${bodyContent}</section> `;

    return html` ${body} ${footer} `;
  }
}
