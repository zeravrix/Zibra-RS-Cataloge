package org.zeravrix.parsers.site.madara.ar

import org.zeravrix.parsers.MangaLoaderContext
import org.zeravrix.parsers.MangaSourceParser
import org.zeravrix.parsers.model.ContentType
import org.zeravrix.parsers.model.MangaParserSource
import org.zeravrix.parsers.site.madara.MadaraParser

@MangaSourceParser("ARABTOONS", "ArabToons", "ar", ContentType.HENTAI)
internal class ArabToons(context: MangaLoaderContext) :
	MadaraParser(context, MangaParserSource.ARABTOONS, "arabtoons.net") {
	override val datePattern = "dd-MM-yyyy"
}
