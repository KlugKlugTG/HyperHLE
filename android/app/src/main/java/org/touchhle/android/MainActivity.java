/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * Parts of this file are derived from SDL 2's Android project template, which
 * has a different license. Please see vendor/SDL/LICENSE.txt for details.
 */
package org.touchhle.android;

import android.app.Activity;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.util.Log;

import org.libsdl.app.SDLActivity;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;

/**
 * A wrapper class over SDLActivity
 */

public class MainActivity extends SDLActivity {
    private static final String TAG = "touchHLE";

    // Message ID sent from the Rust app picker (see window.rs) to open the
    // .ipa file picker. Must match window.rs ADD_IPA_COMMAND.
    private static final int MSG_ADD_IPA = 0x8000;

    // Request code for the system file picker started by this activity.
    private static final int REQUEST_ADD_IPA = 1;

    @Override
    protected String[] getLibraries() {
        return new String[]{
            "SDL2",
            "touchHLE"
        };
    }

    @Override
    protected boolean onUnhandledMessage(int message, Object data) {
        if (message == MSG_ADD_IPA) {
            // The message arrives on SDL's native thread; the file picker
            // must be started from the UI thread.
            runOnUiThread(new Runnable() {
                public void run() {
                    openIpaPicker();
                }
            });
            return true;
        }
        return super.onUnhandledMessage(message, data);
    }

    private void openIpaPicker() {
        Intent intent = new Intent(Intent.ACTION_GET_CONTENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType("*/*");
        intent.putExtra(Intent.EXTRA_MIME_TYPES, new String[]{
            "application/octet-stream", "application/zip",
            "application/x-zip-compressed"});
        try {
            startActivityForResult(
                Intent.createChooser(intent, "Add game (.ipa)"),
                REQUEST_ADD_IPA);
        } catch (Exception e) {
            Log.e(TAG, "Couldn't open file picker", e);
        }
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode != REQUEST_ADD_IPA || resultCode != Activity.RESULT_OK
                || data == null || data.getData() == null) {
            return;
        }
        Uri uri = data.getData();
        String name = displayName(uri);
        if (name == null || name.isEmpty()) {
            name = "game.ipa";
        }
        if (!name.toLowerCase().endsWith(".ipa")) {
            name += ".ipa";
        }
        copyIpa(uri, name);
    }

    private String displayName(Uri uri) {
        ContentResolver resolver = getContentResolver();
        Cursor cursor = null;
        try {
            cursor = resolver.query(uri, null, null, null, null);
            if (cursor != null && cursor.moveToFirst()) {
                int idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (idx >= 0 && !cursor.isNull(idx)) {
                    return cursor.getString(idx);
                }
            }
        } catch (Exception e) {
            Log.w(TAG, "Couldn't query display name", e);
        } finally {
            if (cursor != null) {
                cursor.close();
            }
        }
        String last = uri.getLastPathSegment();
        return last == null ? null : last.substring(last.lastIndexOf('/') + 1);
    }

    private void copyIpa(Uri uri, String name) {
        // touchHLE lists apps from this directory (see APPS_DIR in paths.rs);
        // it matches SDL_AndroidGetExternalStoragePath() on Android.
        File appsDir = new File(getExternalFilesDir(null), "touchHLE_apps");
        if (!appsDir.exists() && !appsDir.mkdirs()) {
            Log.e(TAG, "Couldn't create " + appsDir);
            return;
        }
        InputStream in = null;
        FileOutputStream out = null;
        try {
            in = getContentResolver().openInputStream(uri);
            if (in == null) {
                Log.e(TAG, "Couldn't open " + uri);
                return;
            }
            out = new FileOutputStream(new File(appsDir, name));
            byte[] buffer = new byte[65536];
            int read;
            while ((read = in.read(buffer)) >= 0) {
                out.write(buffer, 0, read);
            }
            out.flush();
            Log.i(TAG, "Added game to " + appsDir + ": " + name);
        } catch (IOException | SecurityException e) {
            Log.e(TAG, "Couldn't copy .ipa file: " + name, e);
        } finally {
            if (in != null) {
                try {
                    in.close();
                } catch (IOException e) {
                    // Nothing to do.
                }
            }
            if (out != null) {
                try {
                    out.close();
                } catch (IOException e) {
                    // Nothing to do.
                }
            }
        }
    }
}
