<?php
namespace Drupal\Core;

class Base
{
    public function base(): void
    {
    }
}

class Named
{
    public function /*@target:named*/named(): void
    {
    }
}

class Autowired
{
    public function autowired(): void
    {
    }
}

class SharedCore
{
    public function share(): void
    {
    }
}
