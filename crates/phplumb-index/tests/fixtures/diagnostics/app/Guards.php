<?php
namespace App;

use Lib\Optional;

if (class_exists(Optional::class)) {
    new Optional();
}
if (class_exists('\Lib\Other')) {
    new \Lib\Other();
}
if (function_exists('maybe')) {
    maybe();
}
if (defined('MAYBE_LIMIT')) {
    echo MAYBE_LIMIT;
}

function guarded(\Lib\Content $content): void
{
    if (method_exists($content, 'later')) {
        $content->later();
    }
}
