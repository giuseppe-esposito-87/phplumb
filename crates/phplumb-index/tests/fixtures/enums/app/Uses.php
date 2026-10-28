<?php

namespace App;

function uses(Severity $severity, Plain $plain): void
{
    Severity::from('low');
    Severity::tryFrom('low');
    Severity::cases();
    $severity->value;
    $plain->name;
    Plain::from('x');
}
