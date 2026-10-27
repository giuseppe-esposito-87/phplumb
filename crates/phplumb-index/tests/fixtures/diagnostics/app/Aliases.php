<?php
namespace App;

trait Builds
{
    public function build(): void
    {
    }
}

class UsesAlias
{
    use Builds {
        build as traitBuild;
    }

    public function build(): void
    {
        $this->traitBuild();
        self::traitBuild();
        $this->/*@broken*/traitBuilt();
    }
}

trait Original
{
    public function original(): void
    {
    }

    public static function staticOriginal(): void
    {
    }
}

trait Wrapper
{
    use Original {
        original as renamed;
        staticOriginal as staticRenamed;
    }
}

class Host
{
    use Wrapper;

    public function run(): void
    {
        $this->renamed();
        self::staticRenamed();
    }
}

function outside(): void
{
    Host::staticRenamed();
}
