param($in, $out, $x, $y, $w, $h)
Add-Type -AssemblyName System.Drawing
$src = [System.Drawing.Bitmap]::FromFile($in)
$dst = $src.Clone((New-Object System.Drawing.Rectangle $x, $y, $w, $h), $src.PixelFormat)
$src.Dispose(); $dst.Save($out)
