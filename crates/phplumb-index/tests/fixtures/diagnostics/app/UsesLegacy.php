<?php
namespace App;

function legacy(): void
{
    new \Lib\Legacy();
    \Lib\Legacy::create();
    \Lib\Legacy::anything();
}
