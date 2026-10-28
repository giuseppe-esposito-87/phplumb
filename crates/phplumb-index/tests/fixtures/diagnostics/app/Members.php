<?php
namespace App;

use Lib\Content;
use Lib\Documented;
use Lib\EntityInterface;
use Lib\Failure;
use Lib\Magic;
use Lib\Mixed;
use Lib\NoConstructor;
use Lib\Plain;
use Lib\StaticMagic;
use Lib\Suit;

class Members extends NoConstructor
{
    public function __construct()
    {
        parent::/*@broken*/__construct();
    }

    public function calls(Content $content, EntityInterface $entity, $unknown, Magic $magic, Documented $documented, Mixed $mixed, Failure $failure, Content|NoConstructor $union, \DateTime $date): void
    {
        $content->get('x');
        $content->id();
        $content->/*@broken*/invented();
        $entity->get('x');
        $entity->/*@broken*/fabricated();
        $unknown->whatever();
        $magic->anything();
        $documented->virtual();
        $documented->undocumented();
        $mixed->anything();
        $failure->getMessage();
        $failure->anything();
        $union->get('x');
        $date->format('Y');
        $date->whatever();
        (new Content())->/*@broken*/notThere();
        StaticMagic::anything();
        Content::create();
        Content::/*@broken*/destroy();
        echo Content::TYPE;
        echo Content::/*@broken*/KIND;
        echo Suit::Hearts->value;
        echo Suit::/*@broken*/Spades;
        Suit::cases();
        Suit::from('h');
        Suit::tryFrom('h');
        Plain::cases();
        Plain::/*@broken*/nowhereDeclared();
        self::helper();
        static::helper();
        self::/*@broken*/absent();
        $this->helper();
        $this->/*@broken*/absent();
        echo Content::class;
    }

    private static function helper(): void
    {
    }
}
