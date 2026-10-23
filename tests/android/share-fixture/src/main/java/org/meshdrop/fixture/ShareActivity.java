package org.meshdrop.fixture;

import android.app.Activity;
import android.content.ClipData;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import java.util.ArrayList;

/** am cannot construct ArrayList<Uri>; this real sender builds the standard extra. */
public final class ShareActivity extends Activity {
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        Uri first = getIntent().getData();
        Intent send = new Intent(getIntent().getAction()).setType("text/plain")
            .setPackage("org.meshdrop.desktop").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        ClipData clip = ClipData.newRawUri("shared document", first);
        if (Intent.ACTION_SEND_MULTIPLE.equals(send.getAction())) {
            Uri second = Uri.parse("content://org.meshdrop.fixture/documents/two");
            ArrayList<Uri> uris = new ArrayList<>();
            uris.add(first); uris.add(second);
            clip.addItem(new ClipData.Item(second));
            send.putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris);
        } else { send.putExtra(Intent.EXTRA_STREAM, first); }
        send.setClipData(clip);
        startActivity(send);
        finish();
    }
}
