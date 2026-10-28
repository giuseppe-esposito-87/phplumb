<?php

namespace App;

function legacy(): void
{
    (new \Worker())->work();
}
