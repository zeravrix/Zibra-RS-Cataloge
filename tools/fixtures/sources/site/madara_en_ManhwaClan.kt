package org.zeravrix.parsers.site.madara.en

import org.zeravrix.parsers.MangaLoaderContext
import org.zeravrix.parsers.MangaSourceParser
import org.zeravrix.parsers.model.MangaParserSource
import org.zeravrix.parsers.site.madara.MadaraParser

@MangaSourceParser("MANHWACLAN", "ManhwaClan", "en")
internal class ManhwaClan(context: MangaLoaderContext) :
	MadaraParser(context, MangaParserSource.MANHWACLAN, "manhwaclan.com", pageSize = 10) {
	override val datePattern = "MMMM dd, yyyy"
}
