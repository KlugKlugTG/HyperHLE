/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package org.touchhle.android;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.widget.Toast;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * Handles the touchhle://add-ipa URL from the app picker's "+" tile.
 *
 * Opens the system document picker so the user can choose an .ipa file, then
 * simply copies the file into the touchHLE_apps directory, where the app
 * picker will find it on the next refresh.
 */
public class AddIpaActivity extends Activity {
    private static final int REQUEST_PICK_IPA = 1001;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        try {
            Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
            intent.addCategory(Intent.CATEGORY_OPENABLE);
            intent.setType("*/*");
            startActivityForResult(
                    Intent.createChooser(intent, "Choose an .ipa file"),
                    REQUEST_PICK_IPA);
        } catch (Exception e) {
            toast("Couldn't open file picker: " + e.getMessage());
            finish();
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_PICK_IPA) {
            finish();
            return;
        }
        Uri uri = (resultCode == RESULT_OK && data != null) ? data.getData() : null;
        if (uri == null) {
            finish();
            return;
        }
        try {
            String copied = copyIpaToAppsDir(uri);
            toast("Copied to touchHLE_apps: " + copied);
        } catch (Exception e) {
            toast("Couldn't copy IPA: " + e.getMessage());
        }
        finish();
    }

    /**
     * Copies the document picked by the user into touchHLE_apps, giving it a
     * unique name with an .ipa extension so touchHLE recognizes it.
     */
    private String copyIpaToAppsDir(Uri uri) throws Exception {
        String name = queryDisplayName(uri);
        if (name == null || name.isEmpty()) {
            name = "game.ipa";
        }
        // Keep only the file name itself, and make sure it ends in ".ipa".
        int slash = Math.max(name.lastIndexOf('/'), name.lastIndexOf('\\'));
        if (slash >= 0) {
            name = name.substring(slash + 1);
        }
        if (!name.toLowerCase().endsWith(".ipa")) {
            name = name + ".ipa";
        }

        File appsDir = new File(getExternalFilesDir(null), "touchHLE_apps");
        if (!appsDir.exists() && !appsDir.mkdirs()) {
            throw new Exception("can't create " + appsDir);
        }

        File dest = new File(appsDir, name);
        int collision = 1;
        while (dest.exists()) {
            String base = name.substring(0, name.length() - 4);
            dest = new File(appsDir, base + " (" + collision + ").ipa");
            collision++;
        }

        InputStream in = getContentResolver().openInputStream(uri);
        if (in == null) {
            throw new Exception("can't open the picked file");
        }
        OutputStream out = new FileOutputStream(dest);
        byte[] buffer = new byte[64 * 1024];
        int len;
        try {
            while ((len = in.read(buffer)) > 0) {
                out.write(buffer, 0, len);
            }
        } finally {
            try {
                in.close();
            } catch (Exception ignored) {
            }
            try {
                out.close();
            } catch (Exception ignored) {
            }
        }
        return dest.getName();
    }

    private String queryDisplayName(Uri uri) {
        try (android.database.Cursor cursor = getContentResolver().query(
                uri, null, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                int idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (idx >= 0 && !cursor.isNull(idx)) {
                    return cursor.getString(idx);
                }
            }
        } catch (Exception ignored) {
        }
        return null;
    }

    private void toast(String message) {
        try {
            Toast.makeText(this, message, Toast.LENGTH_LONG).show();
        } catch (Exception ignored) {
        }
    }
}
