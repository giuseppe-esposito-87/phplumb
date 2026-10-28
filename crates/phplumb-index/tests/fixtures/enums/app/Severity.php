<?php

namespace App;

enum Severity: string
{
    case Low = 'low';
    case High = 'high';

    const LIMIT = self::High;

    public function label(): string
    {
        return ucfirst($this->value);
    }

    public function lowest(): string
    {
        return self::Low->label();
    }
}
