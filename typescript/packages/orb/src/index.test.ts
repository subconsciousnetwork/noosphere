import { NoosphereContext } from './index.js';
import { expect } from '@esm-bundle/chai';
import { SphereFile } from './noosphere.js';

function makeRandomName(prefix: string, randomCharacters: number = 16) {
  const alphabet =
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  const alphabetLength = alphabet.length;

  for (let i = 0; i < randomCharacters; ++i) {
    prefix += alphabet.charAt(Math.floor(alphabetLength * Math.random()));
  }

  return prefix;
}

describe('orb', () => {
  it('can initialize a NoosphereContext', () => {
    const noosphere = new NoosphereContext('foo');

    noosphere.free();
  });

  it('can create a key', async () => {
    const noosphere = new NoosphereContext('foo');
    const keyName = makeRandomName('key');

    expect(await noosphere.hasKey(keyName)).to.be.false;

    await noosphere.createKey(keyName);

    expect(await noosphere.hasKey(keyName)).to.be.true;

    noosphere.free();
  });

  it('can create a sphere', async () => {
    const noosphere = new NoosphereContext('foo');
    const keyName = makeRandomName('key');

    await noosphere.createKey(keyName);

    const receipt = await noosphere.createSphere(keyName);

    expect(receipt.identity).to.be.ok;
    expect(receipt.mnemonic).to.be.ok;

    receipt.free();
    noosphere.free();
  });

  it('can write a file and read it back', async () => {
    const noosphere = new NoosphereContext('foo');
    const keyName = makeRandomName('key');

    await noosphere.createKey(keyName);

    const receipt = await noosphere.createSphere(keyName);

    const identity = receipt.identity;

    const sphere = await noosphere.getSphereContext(identity);

    const fs = await sphere.fs();

    const fileVersion = await fs.writeString(
      'cats',
      'text/subtext',
      "Cat's are great"
    );

    expect(fileVersion).to.be.ok;

    const sphereVersion = await fs.save();

    expect(sphereVersion).to.be.ok;

    const file = await fs.read('cats');

    expect(file).to.be.ok;

    const fileContents = await file?.intoText();

    expect(fileContents).to.be.eq("Cat's are great");

    fs.free();
    sphere.free();
    receipt.free();
    noosphere.free();
  });

  it('can stream all files in the sphere', async () => {
    const noosphere = new NoosphereContext('foo');
    const keyName = makeRandomName('key');

    await noosphere.createKey(keyName);

    const receipt = await noosphere.createSphere(keyName);
    const identity = receipt.identity;

    const sphere = await noosphere.getSphereContext(identity);

    const fs = await sphere.fs();

    for (let i = 0; i < 10; ++i) {
      await fs.writeString('cats' + i, 'text/subtext', "Cat's are great " + i);
    }

    await fs.save();

    const sphereContents: Map<string, SphereFile> = new Map();

    await fs.stream((slug: string, file: SphereFile) => {
      sphereContents.set(slug, file);
    });

    expect(sphereContents.size).to.be.eq(10);

    for (let entry in sphereContents.keys()) {
      let index = entry.slice(3);
      let file = sphereContents.get(entry);

      let text = await file?.intoText();
      expect(text).to.be.eq("Cat's are great " + index);
    }

    fs.free();
    sphere.free();
    receipt.free();
    noosphere.free();
  });
});

describe('HTML conversion', () => {
  it('can generate HTML from text/subtext', async () => {
    const noosphere = new NoosphereContext('foo');
    const keyName = makeRandomName('key');

    await noosphere.createKey(keyName);

    const receipt = await noosphere.createSphere(keyName);

    const identity = receipt.identity;

    const sphere = await noosphere.getSphereContext(identity);

    const fs = await sphere.fs();

    await fs.writeString(
      'cats',
      'text/subtext',
      `# All about cats

Catz r gr8

> Cause ya y not

/cats-r-great`
    );

    await fs.save();

    const file = await fs.read('cats');
    const html = await file?.intoHtml((link: string) => link);

    const expectedHTML = `<article class="subtext"><section class="block"><section class="block-content"><h1 class="block-header"><span class="text">All about cats</span></h1></section></section>

<section class="block"><section class="block-content"><p class="block-paragraph"><span class="text">Catz r gr8</span></p></section></section>

<section class="block"><section class="block-content"><blockquote class="block-quote"><span class="text">Cause ya y not</span></blockquote></section></section>

<section class="block"><section class="block-transcludes"><aside class="transclude"><a class="transclude-format-text" href="/cats-r-great"><span class="link-text">/cats-r-great</span></a></aside></section></section>
</article>`;

    expect(html).to.be.eq(expectedHTML);

    fs.free();
    sphere.free();
    receipt.free();
    noosphere.free();
  });
});
