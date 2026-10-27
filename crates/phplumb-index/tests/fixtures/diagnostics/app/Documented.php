<?php
namespace App;

use Lib\Content;

class DocumentedTypes
{
    /** @var \Lib\Content */
    private $factory;

    public function run(Content $content): void
    {
        $this->factory->rename();
        $this->factory->/*@broken*/zzNowhere();
        $content->format('Y');
    }
}
